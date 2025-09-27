import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import MonacoEditor from '@monaco-editor/react';

type ProcessingStage = "validating" | "downloading";
type StageStatus = "pending" | "active" | "completed" | "error";

interface ProcessingStages {
  validating: StageStatus;
  downloading: StageStatus;
}

interface FileInfo {
  id: string;
  name: string;
  path: string;
}

interface ValidationResult {
  files_to_download: FileInfo[];
  folder_id: string;
}

interface DownloadResult {
  temp_directory: string;
  downloaded_files: FileInfo[];
}

interface ProcessingResult {
  status: string;
  message: string;
  files_processed: number;
  issues_found: number;
  score: number;
  file_paths?: string[];
  analysis_files?: string[];
}

interface FileContent {
  content: string;
  file_type: "text" | "json";
}

interface FileContents {
  base?: FileContent;
  before?: FileContent;
  after?: FileContent;
  agent?: FileContent;
  main_json?: FileContent;
  report?: FileContent;
  analysis?: FileContent;
  base_analysis?: FileContent;
  before_analysis?: FileContent;
  after_analysis?: FileContent;
  agent_analysis?: FileContent;
}

interface AnalysisTableData {
  type: "fail_to_pass" | "pass_to_pass";
  test_name: string;
  base_status: "passed" | "failed" | "non_existing";
  before_status: "passed" | "failed" | "non_existing";
  after_status: "passed" | "failed" | "non_existing";
}

type TabKey = "base" | "before" | "after" | "agent" | "main_json" | "report" | "analysis" | "base_analysis" | "before_analysis" | "after_analysis" | "agent_analysis";
type MainTabKey = "input" | /* "result" | */ "manual_checker"; // COMMENTED OUT AGENTIC FUNCTIONALITY

export default function ReportCheckerPage() {
  const [deliverableLink, setDeliverableLink] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [currentStage, setCurrentStage] = useState<ProcessingStage | null>(null);
  const [stages, setStages] = useState<ProcessingStages>({
    validating: "pending",
    downloading: "pending"
  });
  const [result, setResult] = useState<ProcessingResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabKey>("base");
  const [activeMainTab, setActiveMainTab] = useState<MainTabKey>("manual_checker");
  const [lastActiveTabs, setLastActiveTabs] = useState<{[key in MainTabKey]: TabKey}>({
    input: "base",
    // result: "analysis", // COMMENTED OUT AGENTIC FUNCTIONALITY
    manual_checker: "base"
  });

  const setActiveTabWithMemory = (tabKey: TabKey) => {
    setActiveTab(tabKey);
    setLastActiveTabs(prev => ({
      ...prev,
      [activeMainTab]: tabKey
    }));
  };

  const setActiveMainTabWithMemory = (mainTabKey: MainTabKey) => {
    setActiveMainTab(mainTabKey);
    setActiveTab(lastActiveTabs[mainTabKey]);
  };

  const [fileContents, setFileContents] = useState<FileContents>({});
  const [loadingFiles, setLoadingFiles] = useState(false);
  // COMMENTED OUT - AGENTIC FUNCTIONALITY
  // const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [editableContents, setEditableContents] = useState<{[key in TabKey]?: string}>({});
  const [scrollPositions, setScrollPositions] = useState<{[key in TabKey]?: {scrollTop: number, scrollLeft: number}}>({});
  const editorRefs = useRef<{[key in TabKey]?: any}>({});
  // COMMENTED OUT - AGENTIC FUNCTIONALITY
  // const [analysisTableData, setAnalysisTableData] = useState<AnalysisTableData[]>([]);

  // Manual checker state
  const [failToPassTests, setFailToPassTests] = useState<string[]>([]);
  const [passToPassTests, setPassToPassTests] = useState<string[]>([]);
  const [selectedFailToPassIndex, setSelectedFailToPassIndex] = useState(0);
  const [selectedPassToPassIndex, setSelectedPassToPassIndex] = useState(0);
  const [currentSelection, setCurrentSelection] = useState<"fail_to_pass" | "pass_to_pass">("fail_to_pass");
  
  // Analysis processing state
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [analysisResult, setAnalysisResult] = useState<any>(null);
  
  // Filter state
  const [failToPassFilter, setFailToPassFilter] = useState("");
  const [passToPassFilter, setPassToPassFilter] = useState("");
  
  const [searchResults, setSearchResults] = useState<{
    base: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>,
    before: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>,
    after: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>,
    agent: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>
  }>({ base: [], before: [], after: [], agent: [] });
  const [searchResultIndices, setSearchResultIndices] = useState({
    base: 0,
    before: 0,
    after: 0,
    agent: 0
  });

  // Helper function to get test status from analysis result
  const getTestStatus = (testName: string, testType: "f2p" | "p2p") => {
    if (!analysisResult) return null;
    
    const analysis = testType === "f2p" 
      ? analysisResult.f2p_analysis?.[testName]
      : analysisResult.p2p_analysis?.[testName];
    
    if (!analysis) return null;
    
    return {
      base: analysis.base || "missing",
      before: analysis.before || "missing", 
      after: analysis.after || "missing",
      agent: analysis.agent || "missing"
    };
  };

  // Helper function to check if test has rule violations
  const hasRuleViolations = (testName: string) => {
    if (!analysisResult) return false;
    
    const ruleChecks = analysisResult.rule_checks;
    if (!ruleChecks) return false;
    
    // Check if test appears in any rule violation examples
    for (const ruleKey of Object.keys(ruleChecks)) {
      const rule = ruleChecks[ruleKey];
      if (rule.has_problem && rule.examples && rule.examples.includes(testName)) {
        return true;
      }
    }
    
    return false;
  };

  // Helper function to get test-specific violation details
  const getTestSpecificViolationDetails = (testName: string, testType: "f2p" | "p2p") => {
    const testStatus = getTestStatus(testName, testType);
    if (!testStatus) return [];

    const violations: string[] = [];

    // Check for missing or failed tests in after log
    if (testStatus.after === "missing") {
      violations.push("Test is missing in after log");
    } else if (testStatus.after === "failed") {
      violations.push("Test failed in after log");
    }

    // Check for F2P tests passing in before log (should fail in before)
    // if (testType === "f2p" && testStatus.before === "passed") {
    //   violations.push("F2P test is passing in before log - F2P tests should fail before the fix and pass after");
    // }

    return violations;
  };

  // Helper function to check for additional test-specific violations
  const hasTestSpecificViolations = (testName: string, testType: "f2p" | "p2p") => {
    return getTestSpecificViolationDetails(testName, testType).length > 0;
  };

  // Combined function to check for any violations
  const hasAnyViolations = (testName: string, testType: "f2p" | "p2p") => {
    return hasRuleViolations(testName) || hasTestSpecificViolations(testName, testType);
  };

  // Helper function to get specific error messages for a test
  const getTestErrorMessages = (testName: string): string[] => {
    if (!analysisResult) return [];
    
    const ruleChecks = analysisResult.rule_checks;
    const errorMessages: string[] = [];
    
    // Check for rule violations
    if (ruleChecks) {
      const errorMessageMap: { [key: string]: string } = {
        "c1_failed_in_base_present_in_P2P": "At least one failed test in base log is present in P2P",
        "c2_failed_in_after_present_in_F2P_or_P2P": "At least one failed test in after log is present in F2P / P2P",
        "c3_F2P_success_in_before": "At least one F2P test is present and successful in before log",
        "c4_P2P_missing_in_base_and_not_passing_in_before": "At least one P2P, that is missing in base, and is found but failing in before or is missing from base and before",
        "c5_duplicates_in_same_log_for_F2P_or_P2P": "At least one F2P / P2P test name is duplicated (present 2 times in the same logs)"
      };
      
      // Check if test appears in any rule violation examples
      for (const ruleKey of Object.keys(ruleChecks)) {
        const rule = ruleChecks[ruleKey];
        if (rule.has_problem && rule.examples && rule.examples.includes(testName)) {
          const errorMessage = errorMessageMap[ruleKey];
          if (errorMessage) {
            errorMessages.push(errorMessage);
          }
        }
      }
    }
    
    // Check for test-specific violations using the shared helper
    const testType = currentSelection === "fail_to_pass" ? "f2p" : "p2p";
    const testSpecificViolations = getTestSpecificViolationDetails(testName, testType);
    errorMessages.push(...testSpecificViolations);
    
    return errorMessages;
  };

  // Helper function to render status icon
  const renderStatusIcon = (status: string) => {
    switch (status) {
      case "passed":
        return (
          <div className="w-4 h-4 flex items-center justify-center bg-green-100 dark:bg-green-900/50 rounded-full">
            <svg className="w-2.5 h-2.5 text-green-600 dark:text-green-400" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
            </svg>
          </div>
        );
      case "failed":
        return (
          <div className="w-4 h-4 flex items-center justify-center bg-red-100 dark:bg-red-900/50 rounded-full">
            <svg className="w-2.5 h-2.5 text-red-600 dark:text-red-400" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
            </svg>
          </div>
        );
      case "ignored":
        return (
          <div className="w-4 h-4 flex items-center justify-center bg-yellow-100 dark:bg-yellow-900/50 rounded-full">
            <svg className="w-2.5 h-2.5 text-yellow-600 dark:text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
            </svg>
          </div>
        );
      default: // missing
        return (
          <div className="w-4 h-4 flex items-center justify-center bg-gray-100 dark:bg-gray-700 rounded-full">
            <svg className="w-2.5 h-2.5 text-gray-400 dark:text-gray-500" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-3a1 1 0 00-.867.5 1 1 0 11-1.731-1A3 3 0 0113 8a3.001 3.001 0 01-2 2.83V11a1 1 0 11-2 0v-1a1 1 0 011-1 1 1 0 100-2zm0 8a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
            </svg>
          </div>
        );
    }
  };

  // Filtered test arrays with sorting (errors first)
  const filteredFailToPassTests = failToPassTests
    .filter(test => test.toLowerCase().includes(failToPassFilter.toLowerCase()))
    .sort((a, b) => {
      const aHasError = hasAnyViolations(a, "f2p");
      const bHasError = hasAnyViolations(b, "f2p");
      if (aHasError && !bHasError) return -1;
      if (!aHasError && bHasError) return 1;
      return a.localeCompare(b);
    });
    
  const filteredPassToPassTests = passToPassTests
    .filter(test => test.toLowerCase().includes(passToPassFilter.toLowerCase()))
    .sort((a, b) => {
      const aHasError = hasAnyViolations(a, "p2p");
      const bHasError = hasAnyViolations(b, "p2p");
      if (aHasError && !bHasError) return -1;
      if (!aHasError && bHasError) return 1;
      return a.localeCompare(b);
    });

  // Reset selection indices when filters change
  useEffect(() => {
    if (selectedFailToPassIndex >= filteredFailToPassTests.length) {
      setSelectedFailToPassIndex(0);
    }
  }, [failToPassFilter, filteredFailToPassTests.length, selectedFailToPassIndex]);

  useEffect(() => {
    if (selectedPassToPassIndex >= filteredPassToPassTests.length) {
      setSelectedPassToPassIndex(0);
    }
  }, [passToPassFilter, filteredPassToPassTests.length, selectedPassToPassIndex]);

  const resetState = () => {
    setDeliverableLink("");
    setIsProcessing(false);
    setCurrentStage(null);
    setStages({
      validating: "pending",
      downloading: "pending"
    });
    setResult(null);
    setError(null);
    setActiveTab("base");
    setActiveMainTab("manual_checker");
    setLastActiveTabs({
      input: "base",
      // result: "analysis", // COMMENTED OUT AGENTIC FUNCTIONALITY
      manual_checker: "base"
    });
    setFileContents({});
    setLoadingFiles(false);
    // setIsAnalyzing(false); // COMMENTED OUT - AGENTIC FUNCTIONALITY
    setEditableContents({});
    setScrollPositions({});
    editorRefs.current = {};
    // setAnalysisTableData([]); // COMMENTED OUT - AGENTIC FUNCTIONALITY
    // Reset manual checker state
    setFailToPassTests([]);
    setPassToPassTests([]);
    setSelectedFailToPassIndex(0);
    setSelectedPassToPassIndex(0);
    setCurrentSelection("fail_to_pass");
    setSearchResults({ base: [], before: [], after: [], agent: [] });
    setSearchResultIndices({ base: 0, before: 0, after: 0, agent: 0 });
    // Reset filter state
    setFailToPassFilter("");
    setPassToPassFilter("");
    // Reset analysis state
    setIsAnalyzing(false);
    setAnalysisResult(null);
  };

  const updateStageStatus = (stage: ProcessingStage, status: StageStatus) => {
    setStages(prev => ({
      ...prev,
      [stage]: status
    }));
  };

  const loadFileContents = async () => {
    if (!result?.file_paths || result.file_paths.length === 0) return;
    
    setLoadingFiles(true);
    console.log("Loading file contents for result:", result);
    
    try {
      const contents: FileContents = {};
      
      // Load each file type
      const fileTypes = ["base", "before", "after", "agent", "main_json", "report"];
      
      for (const fileType of fileTypes) {
        try {
          const content = await invoke("get_file_content", { 
            fileType: fileType,
            filePaths: result.file_paths 
          }) as string;
          
          // Determine file type - only JSON files are treated as JSON
          const isJsonType = fileType.includes("json") || fileType === "report";
          contents[fileType as TabKey] = {
            content,
            file_type: isJsonType ? "json" : "text"
          };
          console.log(`Loaded ${fileType} file, content length: ${content.length}`);
        } catch (error) {
          console.warn(`Failed to load ${fileType}:`, error);
        }
      }
      
      // Load analysis files if analysis was performed
      console.log("Processing analysis content...");
      if (result.status === "rejected") {
        console.log("Analysis was rejected, adding rejection message");
        contents["analysis"] = {
          content: result.message,
          file_type: "text"
        };
      } else if (result.analysis_files && result.analysis_files.length > 0) {
        console.log("Loading analysis files:", result.analysis_files);
        // Load individual analysis files
        for (const analysisPath of result.analysis_files) {
          try {
            console.log(`Loading analysis file: ${analysisPath}`);
            const content = await invoke("read_analysis_file", { filePath: analysisPath }) as string;
            console.log(`Analysis file loaded, content length: ${content.length}`);
            
            // Determine which log type this analysis corresponds to
            let analysisKey: TabKey = "analysis";
            if (analysisPath.includes("base")) {
              analysisKey = "base_analysis";
            } else if (analysisPath.includes("before")) {
              analysisKey = "before_analysis";
            } else if (analysisPath.includes("after")) {
              analysisKey = "after_analysis";
            } else if (analysisPath.includes("post_agent_patch")) {
              analysisKey = "agent_analysis";
            }
            
            contents[analysisKey] = {
              content,
              file_type: "json"
            };
            console.log(`Analysis content added for key: ${analysisKey}`);
          } catch (error) {
            console.error(`Failed to load analysis file ${analysisPath}:`, error);
            // Add error content to analysis tab
            contents["analysis"] = {
              content: `Failed to load analysis file ${analysisPath}: ${error}`,
              file_type: "text"
            };
          }
        }
      } else {
        console.log("No analysis files, adding default analysis tab with result info");
        // Add analysis tab with whatever result we have
        contents["analysis"] = {
          content: `Analysis completed with status: ${result.status}\nMessage: ${result.message}\nAnalysis files: ${result.analysis_files ? JSON.stringify(result.analysis_files, null, 2) : 'None'}`,
          file_type: "text"
        };
      }
      
      console.log("Final contents object:", contents);
      setFileContents(contents);
      
      // Initialize editable contents for JSON tabs
      const editableInit: {[key in TabKey]?: string} = {};
      for (const key of ["main_json", "report"] as TabKey[]) {
        if (contents[key] && contents[key].file_type === "json") {
          editableInit[key] = contents[key].content;
        }
      }
      setEditableContents(editableInit);
    } catch (error) {
      console.error("Failed to load file contents:", error);
      // Even on error, add analysis tab with error info
      setFileContents({
        "analysis": {
          content: `Error loading file contents: ${error}`,
          file_type: "text"
        }
      });
    } finally {
      setLoadingFiles(false);
    }
  };

  // Load file contents when result is available
  useEffect(() => {
    if (result) {
      loadFileContents();
      loadTestLists();
    }
  }, [result]);

  // Start analysis when Tests Checker tab is opened
  useEffect(() => {
    if (activeMainTab === "manual_checker" && result && !isAnalyzing && !analysisResult) {
      startAnalysis();
    }
  }, [activeMainTab, result, isAnalyzing, analysisResult]);

  // Parse analysis data when analysis files are loaded
  useEffect(() => {
    if (fileContents.base_analysis && fileContents.before_analysis && fileContents.after_analysis) {
      parseAnalysisData();
    }
  }, [fileContents.base_analysis, fileContents.before_analysis, fileContents.after_analysis]);

  const parseAnalysisData = () => {
    try {
      if (!fileContents.base_analysis || !fileContents.before_analysis || !fileContents.after_analysis) {
        return;
      }

      const baseData = JSON.parse(fileContents.base_analysis.content);
      const beforeData = JSON.parse(fileContents.before_analysis.content);
      const afterData = JSON.parse(fileContents.after_analysis.content);

      // Get test list from main.json
      const mainJson = fileContents.main_json ? JSON.parse(fileContents.main_json.content) : {};
      const failToPass = mainJson.fail_to_pass || [];
      const passToPass = mainJson.pass_to_pass || [];

      const tableData: AnalysisTableData[] = [];

      // Process fail_to_pass tests first
      for (const testName of failToPass) {
        const baseStatus = findTestStatus(baseData, testName);
        const beforeStatus = findTestStatus(beforeData, testName);
        const afterStatus = findTestStatus(afterData, testName);

        tableData.push({
          type: "fail_to_pass",
          test_name: testName,
          base_status: baseStatus,
          before_status: beforeStatus,
          after_status: afterStatus
        });
      }

      // Process pass_to_pass tests second
      for (const testName of passToPass) {
        const baseStatus = findTestStatus(baseData, testName);
        const beforeStatus = findTestStatus(beforeData, testName);
        const afterStatus = findTestStatus(afterData, testName);

        tableData.push({
          type: "pass_to_pass",
          test_name: testName,
          base_status: baseStatus,
          before_status: beforeStatus,
          after_status: afterStatus
        });
      }

      // setAnalysisTableData(tableData); // COMMENTED OUT - AGENTIC FUNCTIONALITY
    } catch (error) {
      console.error("Failed to parse analysis data:", error);
    }
  };

  const findTestStatus = (analysisData: any, testName: string): "passed" | "failed" | "non_existing" => {
    // Handle both old format (array) and new structured format (object with test_results)
    let testResults: any[];
    if (Array.isArray(analysisData)) {
      testResults = analysisData;
    } else if (analysisData?.test_results && Array.isArray(analysisData.test_results)) {
      testResults = analysisData.test_results;
    } else {
      return "non_existing";
    }
    
    const test = testResults.find((item: any) => item.test_name === testName);
    if (!test) return "non_existing";
    
    if (test.status === "passed") return "passed";
    if (test.status === "failed") return "failed";
    return "non_existing";
  };

  const loadTestLists = async () => {
    if (!result?.file_paths || result.file_paths.length === 0) return;
    
    try {
      const testLists = await invoke("get_test_lists", { filePaths: result.file_paths }) as {
        fail_to_pass: string[],
        pass_to_pass: string[]
      };
      
      setFailToPassTests(testLists.fail_to_pass);
      setPassToPassTests(testLists.pass_to_pass);
      
      // Reset selection indices
      setSelectedFailToPassIndex(0);
      setSelectedPassToPassIndex(0);
      
      // Load search results for the first test
      if (testLists.fail_to_pass.length > 0) {
        await searchForTest(testLists.fail_to_pass[0]);
      } else if (testLists.pass_to_pass.length > 0) {
        setCurrentSelection("pass_to_pass");
        await searchForTest(testLists.pass_to_pass[0]);
      }
    } catch (error) {
      console.error("Failed to load test lists:", error);
    }
  };

  const startAnalysis = async () => {
    if (!result?.file_paths || result.file_paths.length === 0) return;
    
    setIsAnalyzing(true);
    setError(null);
    
    try {
      console.log("Starting log analysis with file paths:", result.file_paths);
      const analysisData = await invoke("analyze_logs", { filePaths: result.file_paths }) as any;
      console.log("Analysis completed:", analysisData);
      setAnalysisResult(analysisData);
    } catch (error) {
      console.error("Analysis failed:", error);
      setError(`Analysis failed: ${error}`);
    } finally {
      setIsAnalyzing(false);
    }
  };

  const searchForTest = async (testName: string) => {
    if (!result?.file_paths || result.file_paths.length === 0) return;
    
    try {
      const results = await invoke("search_logs", { 
        filePaths: result.file_paths, 
        testName: testName 
      }) as {
        base_results: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>,
        before_results: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>,
        after_results: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>,
        agent_results: Array<{line_number: number, line_content: string, context_before: string[], context_after: string[]}>
      };
      
      setSearchResults({
        base: results.base_results,
        before: results.before_results,
        after: results.after_results,
        agent: results.agent_results
      });
      
      // Reset search result indices
      setSearchResultIndices({ base: 0, before: 0, after: 0, agent: 0 });
    } catch (error) {
      console.error("Failed to search logs:", error);
      setSearchResults({ base: [], before: [], after: [], agent: [] });
    }
  };

  // Force re-render when fileContents changes to update tabs
  useEffect(() => {
    // This will trigger a re-render when fileContents changes
  }, [fileContents]);

  // Keyboard navigation for manual checker
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (activeMainTab !== "manual_checker") return;
      
      if (event.key === "ArrowUp") {
        event.preventDefault();
        if (currentSelection === "fail_to_pass") {
          const newIndex = Math.max(0, selectedFailToPassIndex - 1);
          setSelectedFailToPassIndex(newIndex);
          if (filteredFailToPassTests[newIndex]) {
            searchForTest(filteredFailToPassTests[newIndex]);
          }
        } else {
          const newIndex = Math.max(0, selectedPassToPassIndex - 1);
          setSelectedPassToPassIndex(newIndex);
          if (filteredPassToPassTests[newIndex]) {
            searchForTest(filteredPassToPassTests[newIndex]);
          }
        }
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        if (currentSelection === "fail_to_pass") {
          const newIndex = Math.min(filteredFailToPassTests.length - 1, selectedFailToPassIndex + 1);
          setSelectedFailToPassIndex(newIndex);
          if (filteredFailToPassTests[newIndex]) {
            searchForTest(filteredFailToPassTests[newIndex]);
          }
        } else {
          const newIndex = Math.min(filteredPassToPassTests.length - 1, selectedPassToPassIndex + 1);
          setSelectedPassToPassIndex(newIndex);
          if (filteredPassToPassTests[newIndex]) {
            searchForTest(filteredPassToPassTests[newIndex]);
          }
        }
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        // Navigate between fail_to_pass and pass_to_pass
        if (currentSelection === "pass_to_pass" && failToPassTests.length > 0) {
          setCurrentSelection("fail_to_pass");
          if (failToPassTests[selectedFailToPassIndex]) {
            searchForTest(failToPassTests[selectedFailToPassIndex]);
          }
        }
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        // Navigate between fail_to_pass and pass_to_pass
        if (currentSelection === "fail_to_pass" && passToPassTests.length > 0) {
          setCurrentSelection("pass_to_pass");
          if (passToPassTests[selectedPassToPassIndex]) {
            searchForTest(passToPassTests[selectedPassToPassIndex]);
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeMainTab, currentSelection, selectedFailToPassIndex, selectedPassToPassIndex, failToPassTests, passToPassTests]);

  // Auto-scroll to selected item when selection changes
  useEffect(() => {
    if (activeMainTab !== "manual_checker") return;
    
    const scrollToSelectedItem = () => {
      const selectedIndex = currentSelection === "fail_to_pass" ? selectedFailToPassIndex : selectedPassToPassIndex;
      const elementId = `${currentSelection}-item-${selectedIndex}`;
      const element = document.getElementById(elementId);
      
      if (element) {
        element.scrollIntoView({
          behavior: "smooth",
          block: "nearest",
          inline: "nearest"
        });
      }
    };
    
    // Small delay to ensure DOM is updated
    const timeoutId = setTimeout(scrollToSelectedItem, 50);
    return () => clearTimeout(timeoutId);
  }, [activeMainTab, currentSelection, selectedFailToPassIndex, selectedPassToPassIndex]);

  const handleSearchResultNavigation = (logType: "base" | "before" | "after" | "agent", direction: "next" | "prev") => {
    const currentResults = searchResults[logType];
    if (currentResults.length === 0) return;
    
    const currentIndex = searchResultIndices[logType];
    let newIndex;
    
    if (direction === "next") {
      newIndex = (currentIndex + 1) % currentResults.length;
    } else {
      newIndex = currentIndex === 0 ? currentResults.length - 1 : currentIndex - 1;
    }
    
    setSearchResultIndices(prev => ({
      ...prev,
      [logType]: newIndex
    }));
  };

  const copyTestName = async () => {
    const testName = currentSelection === "fail_to_pass" 
      ? filteredFailToPassTests[selectedFailToPassIndex]
      : filteredPassToPassTests[selectedPassToPassIndex];
    
    if (testName) {
      try {
        await navigator.clipboard.writeText(testName);
        // Could add a toast notification here if desired
        console.log("Test name copied to clipboard:", testName);
      } catch (err) {
        console.error("Failed to copy test name:", err);
        // Fallback for older browsers
        const textArea = document.createElement("textarea");
        textArea.value = testName;
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();
        try {
          document.execCommand('copy');
          console.log("Test name copied to clipboard (fallback):", testName);
        } catch (fallbackErr) {
          console.error("Fallback copy failed:", fallbackErr);
        }
        document.body.removeChild(textArea);
      }
    }
  };

  /* COMMENTED OUT - AGENTIC FUNCTIONALITY
  const handleAnalyze = async () => {
    if (!result?.file_paths) return;
    
    setIsAnalyzing(true);
    setError(null); // Clear previous errors
    console.log("Starting analysis with file paths:", result.file_paths);
    
    try {
      console.log("Calling backend analyze_files command...");
      const analysisResult = await invoke("analyze_files", { filePaths: result.file_paths }) as {
        status: string;
        message: string;
        analysis_files?: string[];
      };
      
      console.log("Backend analysis result received:", analysisResult);
      console.log("Result type:", typeof analysisResult);
      console.log("Result keys:", Object.keys(analysisResult));
      
      // Update the result with analysis information
      setResult(prev => prev ? {
        ...prev,
        status: analysisResult.status,
        message: analysisResult.message,
        analysis_files: analysisResult.analysis_files,
        issues_found: 3,
        score: 85
      } : null);
      
      console.log("Result state updated, switching to result tab...");
      
      // COMMENTED OUT - AGENTIC FUNCTIONALITY
      // Always switch to result tab to show the analysis
      // setActiveMainTab("result");
      
      // Load analysis files directly instead of calling loadFileContents
      console.log("Loading analysis files directly...");
      if (analysisResult.status === "accepted" && analysisResult.analysis_files) {
        const contents: FileContents = { ...fileContents };
        
        console.log("Analysis files to load:", analysisResult.analysis_files);
        
        // Load each analysis file
        for (const analysisPath of analysisResult.analysis_files) {
          try {
            console.log(`Loading analysis file: ${analysisPath}`);
            const content = await invoke("read_analysis_file", { filePath: analysisPath }) as string;
            console.log(`Analysis file loaded, content length: ${content.length}`);
            
            // Determine which log type this analysis corresponds to
            let analysisKey: TabKey = "analysis";
            if (analysisPath.includes("base")) {
              analysisKey = "base_analysis";
            } else if (analysisPath.includes("before")) {
              analysisKey = "before_analysis";
            } else if (analysisPath.includes("after")) {
              analysisKey = "after_analysis";
            } else if (analysisPath.includes("post_agent_patch")) {
              analysisKey = "agent_analysis";
            }
            
            contents[analysisKey] = {
              content,
              file_type: "json"
            };
            console.log(`Analysis content added for key: ${analysisKey}`);
          } catch (error) {
            console.error(`Failed to load analysis file ${analysisPath}:`, error);
            // Add error content to analysis tab
            contents["analysis"] = {
              content: `Failed to load analysis file ${analysisPath}: ${error}`,
              file_type: "text"
            };
          }
        }
        
        // Update file contents with analysis results
        console.log("Final contents object:", contents);
        setFileContents(contents);
      }
      
    } catch (error: any) {
      console.error("Analysis failed with error:", error);
      console.error("Error type:", typeof error);
      console.error("Error message:", error.message);
      console.error("Error stack:", error.stack);
      setError(`Analysis failed: ${error}`);
      
      // COMMENTED OUT - AGENTIC FUNCTIONALITY
      // Even on error, switch to result tab to show the error
      // setActiveMainTab("result");
    } finally {
      // setIsAnalyzing(false); // COMMENTED OUT - AGENTIC FUNCTIONALITY
    }
  };
  END COMMENTED OUT - AGENTIC FUNCTIONALITY */

  /* COMMENTED OUT - AGENTIC FUNCTIONALITY
  const handleCancelAnalyze = () => {
    setIsAnalyzing(false);
    // In a real implementation, you'd cancel the backend operation here
  };
  END COMMENTED OUT - AGENTIC FUNCTIONALITY */

  const renderJsonEditor = (tabKey: TabKey, content: string) => {
    const handleEditorChange = (value: string | undefined) => {
      setEditableContents(prev => ({
        ...prev,
        [tabKey]: value || ""
      }));
    };

    const handleEditorDidMount = (editor: any) => {
      // Store editor reference
      editorRefs.current[tabKey] = editor;
      
      // Add scroll listener to save positions
      editor.onDidScrollChange(() => {
        const scrollTop = editor.getScrollTop();
        const scrollLeft = editor.getScrollLeft();
        setScrollPositions(prev => ({
          ...prev,
          [tabKey]: { scrollTop, scrollLeft }
        }));
      });
      
      // Restore scroll position if it exists
      const savedPosition = scrollPositions[tabKey];
      if (savedPosition) {
        setTimeout(() => {
          editor.setScrollTop(savedPosition.scrollTop);
          editor.setScrollLeft(savedPosition.scrollLeft);
        }, 100);
      }
    };

    return (
      <div className="h-full flex flex-col">
        <div className="flex-1 border rounded-lg overflow-hidden">
          <MonacoEditor
            key={`json-editor-${tabKey}`}
            height="100%"
            defaultLanguage="json"
            language="json"
            value={editableContents[tabKey] || content}
            onChange={handleEditorChange}
            onMount={handleEditorDidMount}
            options={{
              readOnly: false,
              minimap: { enabled: false },
              fontSize: 14,
              wordWrap: "on",
              automaticLayout: true,
              scrollBeyondLastLine: false,
              folding: true,
              lineNumbers: "on",
              glyphMargin: false,
              lineDecorationsWidth: 0,
              lineNumbersMinChars: 3
            }}
            theme="vs-dark"
          />
        </div>
      </div>
    );
  };

  const renderJsonContent = (content: string, tabKey?: TabKey) => {
    // Use Monaco Editor for main_json tab
    if (tabKey && tabKey === "main_json") {
      return renderJsonEditor(tabKey, content);
    }

    // Use simple pre for other JSON content
    try {
      const formatted = JSON.stringify(JSON.parse(content), null, 2);
      return (
        <pre className="bg-gray-900 text-green-400 p-4 rounded-lg text-sm font-mono whitespace-pre-wrap h-full">
          {formatted}
        </pre>
      );
    } catch (error) {
      return (
        <pre className="bg-gray-100 dark:bg-gray-800 p-4 rounded-lg text-sm font-mono whitespace-pre-wrap h-full">
          {content}
        </pre>
      );
    }
  };

  /* COMMENTED OUT - AGENTIC FUNCTIONALITY
  const renderAnalysisTable = () => {
    if (analysisTableData.length === 0) {
      return (
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <svg className="w-12 h-12 text-gray-400 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <p className="text-gray-500 dark:text-gray-400">
              No analysis data available yet. Analysis files are still being processed.
            </p>
          </div>
        </div>
      );
    }

    const getStatusIcon = (status: string) => {
      switch (status) {
        case "passed":
          return (
            <div className="flex items-center gap-2 text-green-700 dark:text-green-300 font-medium">
              <div className="flex items-center justify-center w-5 h-5 bg-green-100 dark:bg-green-900/50 rounded-full">
                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                </svg>
              </div>
              Passed
            </div>
          );
        case "failed":
          return (
            <div className="flex items-center gap-2 text-red-700 dark:text-red-300 font-medium">
              <div className="flex items-center justify-center w-5 h-5 bg-red-100 dark:bg-red-900/50 rounded-full">
                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
                </svg>
              </div>
              Failed
            </div>
          );
        default:
          return (
            <div className="flex items-center gap-2 text-gray-600 dark:text-gray-400 font-medium">
              <div className="flex items-center justify-center w-5 h-5 bg-gray-100 dark:bg-gray-700 rounded-full">
                <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-3a1 1 0 00-.867.5 1 1 0 11-1.731-1A3 3 0 0113 8a3.001 3.001 0 01-2 2.83V11a1 1 0 11-2 0v-1a1 1 0 011-1 1 1 0 100-2zm0 8a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
                </svg>
              </div>
              Not Found
            </div>
          );
      }
    };

    const getStatusBg = (status: string) => {
      switch (status) {
        case "passed": return "bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800";
        case "failed": return "bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800";
        default: return "bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700";
      }
    };

    return (
      <div className="w-full h-full overflow-auto">
        <table className="w-full border-collapse">
          <thead className="bg-gray-50 dark:bg-gray-700 sticky top-0">
            <tr>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600">
                Type
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600">
                Test Name
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600">
                Base
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600">
                Before
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider border-b border-gray-200 dark:border-gray-600">
                After
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
            {analysisTableData.map((row, index) => (
              <tr key={index} className="hover:bg-gray-50 dark:hover:bg-gray-800">
                <td className="px-4 py-2 text-sm font-medium text-gray-900 dark:text-white">
                  <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
                    row.type === "fail_to_pass" 
                      ? "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200" 
                      : "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200"
                  }`}>
                    {row.type.replace("_", " ").toUpperCase()}
                  </span>
                </td>
                <td className="px-4 py-2 text-sm text-gray-900 dark:text-white font-mono">
                  {row.test_name}
                </td>
                <td className="px-4 py-3">
                  <div className={`px-3 py-2 rounded-lg ${getStatusBg(row.base_status)}`}>
                    {getStatusIcon(row.base_status)}
                  </div>
                </td>
                <td className="px-4 py-3">
                  <div className={`px-3 py-2 rounded-lg ${getStatusBg(row.before_status)}`}>
                    {getStatusIcon(row.before_status)}
                  </div>
                </td>
                <td className="px-4 py-3">
                  <div className={`px-3 py-2 rounded-lg ${getStatusBg(row.after_status)}`}>
                    {getStatusIcon(row.after_status)}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  };
  END COMMENTED OUT - AGENTIC FUNCTIONALITY */

  const renderTextEditor = (content: string, tabKey: TabKey) => {
    const handleEditorDidMount = (editor: any) => {
      // Store editor reference
      editorRefs.current[tabKey] = editor;
      
      // Add scroll listener to save positions
      editor.onDidScrollChange(() => {
        const scrollTop = editor.getScrollTop();
        const scrollLeft = editor.getScrollLeft();
        setScrollPositions(prev => ({
          ...prev,
          [tabKey]: { scrollTop, scrollLeft }
        }));
      });
      
      // Restore scroll position if it exists
      const savedPosition = scrollPositions[tabKey];
      if (savedPosition) {
        setTimeout(() => {
          editor.setScrollTop(savedPosition.scrollTop);
          editor.setScrollLeft(savedPosition.scrollLeft);
        }, 100);
      }
    };

    return (
      <div className="h-full flex flex-col">
        <div className="flex-1 border rounded-lg overflow-hidden">
          <MonacoEditor
            key={`text-editor-${tabKey}`}
            height="100%"
            defaultLanguage="plaintext"
            language="plaintext"
            value={content}
            onMount={handleEditorDidMount}
            options={{
              readOnly: true,
              minimap: { enabled: false },
              fontSize: 14,
              wordWrap: "on",
              automaticLayout: true,
              scrollBeyondLastLine: false,
              folding: true,
              lineNumbers: "on",
              glyphMargin: false,
              lineDecorationsWidth: 0,
              lineNumbersMinChars: 3,
              find: {
                addExtraSpaceOnTop: false,
                autoFindInSelection: "never",
                seedSearchStringFromSelection: "selection"
              }
            }}
            theme="vs-dark"
          />
        </div>
      </div>
    );
  };

  const getInputTabs = () => [
    { key: "base" as TabKey, label: "Base" },
    { key: "before" as TabKey, label: "Before" },
    { key: "after" as TabKey, label: "After" },
    { key: "agent" as TabKey, label: "Agent" },
    { key: "main_json" as TabKey, label: "Main Json" },
    { key: "report" as TabKey, label: "Report" },
  ];

  const inputTabs = getInputTabs();

  const handleSubmit = async () => {
    const trimmedLink = deliverableLink.trim();
    if (!trimmedLink) {
      setError("Please enter a deliverable link");
      return;
    }

    setIsProcessing(true);
    setError(null);
    setResult(null);

    try {
      // Stage 1: Validating
      setCurrentStage("validating");
      updateStageStatus("validating", "active");
      const validationData = await invoke("validate_deliverable", { folderLink: trimmedLink }) as ValidationResult;
      updateStageStatus("validating", "completed");

      // Stage 2: Downloading
      setCurrentStage("downloading");
      updateStageStatus("downloading", "active");
      const downloadData = await invoke("download_deliverable", { filesToDownload: validationData.files_to_download, folderId: validationData.folder_id }) as DownloadResult;
      updateStageStatus("downloading", "completed");

      // Create a simple result with the downloaded files
      const result: ProcessingResult = {
        status: "downloaded",
        message: "Files downloaded successfully. Click Analyze to process them.",
        files_processed: downloadData.downloaded_files.length,
        issues_found: 0,
        score: 0,
        file_paths: downloadData.downloaded_files.map(file => file.path)
      };

      setResult(result);
      setActiveMainTab("manual_checker");
      setCurrentStage(null);
    } catch (error: any) {
      setError(error || "An error occurred during processing");
      if (currentStage) {
        updateStageStatus(currentStage, "error");
      }
      setCurrentStage(null);
    } finally {
      setIsProcessing(false);
    }
  };

  const renderIcon = (stage: ProcessingStage) => {
    const status = stages[stage];
    
    if (status === "completed") {
      return (
        <svg className="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
        </svg>
      );
    }
    
    if (status === "active") {
      return (
        <div className="w-5 h-5">
          <svg className="animate-spin text-blue-500" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
        </div>
      );
    }

    if (status === "error") {
      return (
        <svg className="w-5 h-5 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      );
    }
    
    // pending
    return (
      <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    );
  };

  const getStageTextClass = (stage: ProcessingStage) => {
    const status = stages[stage];
    if (status === "completed") return "text-green-600 dark:text-green-400";
    if (status === "active") return "text-blue-600 dark:text-blue-400";
    if (status === "error") return "text-red-600 dark:text-red-400";
    return "text-gray-400 dark:text-gray-500";
  };

  if (result) {
    return (
      <div className="flex flex-col h-full overflow-hidden">
        <div className="flex-none bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 px-4 py-1 shadow-sm mb-1">
          {/* Single line with back button, centered tabs, and copy functionality */}
          <div className="flex items-center justify-between relative">
            {/* Back button */}
            <button
              onClick={resetState}
              disabled={isAnalyzing}
              className={`flex items-center gap-2 transition-colors text-sm whitespace-nowrap ${
                isAnalyzing 
                  ? "text-gray-400 dark:text-gray-600 cursor-not-allowed" 
                  : "text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300"
              }`}
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 19l-7-7m0 0l7-7m-7 7h18" />
              </svg>
              Check another
            </button>

            {/* Main Tab Navigation - Centered and Larger */}
            <div className="flex justify-center absolute left-1/2 transform -translate-x-1/2">
              <div className="flex space-x-1 bg-gray-100 dark:bg-gray-700 p-1 rounded">
                <button
                  onClick={() => setActiveMainTabWithMemory("manual_checker")}
                  className={`px-6 py-2.5 rounded font-medium text-sm transition-all duration-200 flex items-center gap-2 ${
                    activeMainTab === "manual_checker"
                      ? "bg-white dark:bg-gray-800 text-blue-600 dark:text-blue-400 shadow-sm"
                      : "text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200 dark:hover:bg-gray-600"
                  }`}
                >
                  Tests Checker
                  {isAnalyzing && activeMainTab === "manual_checker" && (
                    <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                  )}
                </button>
                {/* COMMENTED OUT - AGENTIC FUNCTIONALITY */}
                {/*
                <button
                  onClick={() => setActiveMainTabWithMemory("result")}
                  className={`px-6 py-2.5 rounded font-medium text-sm transition-all duration-200 ${
                    activeMainTab === "result"
                      ? "bg-white dark:bg-gray-800 text-blue-600 dark:text-blue-400 shadow-sm"
                      : "text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200 dark:hover:bg-gray-600"
                  }`}
                >
                  Result
                </button>
                */}
                <button
                  onClick={() => setActiveMainTabWithMemory("input")}
                  className={`px-6 py-2.5 rounded font-medium text-sm transition-all duration-200 ${
                    activeMainTab === "input"
                      ? "bg-white dark:bg-gray-800 text-blue-600 dark:text-blue-400 shadow-sm"
                      : "text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200 dark:hover:bg-gray-600"
                  }`}
                >
                  Input
                </button>
              </div>
            </div>

            {/* Copy Selected Test Name - Only show in Tests Checker tab */}
            {activeMainTab === "manual_checker" && (failToPassTests.length > 0 || passToPassTests.length > 0) && (
              <div className="flex flex-col gap-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm text-gray-600 dark:text-gray-400 font-mono max-w-xs truncate">
                    {currentSelection === "fail_to_pass" 
                      ? filteredFailToPassTests[selectedFailToPassIndex]
                      : filteredPassToPassTests[selectedPassToPassIndex]
                    }
                  </span>
                  <button
                    onClick={copyTestName}
                    className="p-1.5 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors"
                    title="Copy test name"
                  >
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                    </svg>
                  </button>
                </div>
                
                {/* Error Messages Display */}
                {(() => {
                  const currentTestName = currentSelection === "fail_to_pass" 
                    ? filteredFailToPassTests[selectedFailToPassIndex]
                    : filteredPassToPassTests[selectedPassToPassIndex];
                  const errorMessages = getTestErrorMessages(currentTestName);
                  
                  if (errorMessages.length > 0) {
                    return (
                      <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-md px-2">
                        <div className="flex items-start gap-0">
    
                          <div className="flex-1">
                            <div className="space-y-0">
                              {errorMessages.map((message, index) => (
                                <div key={index} className="text-xs text-red-700 dark:text-red-300">
                                  {message}
                                </div>
                              ))}
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  }
                  return null;
                })()}
              </div>
            )}

            {/* COMMENTED OUT - AGENTIC FUNCTIONALITY */}
            {/* Analyze/Cancel button */}
            {/*
            {isAnalyzing ? (
              <button
                onClick={handleCancelAnalyze}
                className="flex items-center gap-2 px-3 py-1.5 bg-red-600 hover:bg-red-700 text-white rounded text-sm transition-colors whitespace-nowrap"
              >
                <svg className="animate-spin w-3 h-3" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                Cancel
              </button>
            ) : (
              <button
                onClick={handleAnalyze}
                className="flex items-center gap-2 px-3 py-1.5 bg-green-600 hover:bg-green-700 text-white rounded text-sm transition-colors whitespace-nowrap"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                Analyze
              </button>
            )}
            */}
          </div>
        </div>

        {/* Main Tab Content */}
        <div className="flex-1 overflow-hidden bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 shadow-sm">
          {activeMainTab === "input" ? (
            <div className="flex h-full">
              {/* Vertical Input Tabs */}
              <div className="w-48 bg-gray-100 dark:bg-gray-700 border-r border-gray-200 dark:border-gray-600 flex flex-col">
                {inputTabs.map((tab) => (
                  <button
                    key={tab.key}
                    onClick={() => setActiveTabWithMemory(tab.key)}
                    className={`px-4 py-3 text-left text-sm font-medium transition-all duration-200 ${
                      activeTab === tab.key
                        ? "bg-white dark:bg-gray-800 text-blue-600 dark:text-blue-400 border-r-2 border-blue-500"
                        : "text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200 dark:hover:bg-gray-600"
                    }`}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>
              
              {/* Input Tab Content */}
              <div className="flex-1 flex flex-col p-4">
                {loadingFiles ? (
                  <div className="flex items-center justify-center h-full">
                    <div className="flex items-center gap-3">
                      <svg className="animate-spin w-6 h-6 text-blue-500" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                      </svg>
                      <span className="text-gray-600 dark:text-gray-400">Loading file contents...</span>
                    </div>
                  </div>
                ) : (
                  <>
                    {fileContents[activeTab] ? (
                      <>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-3 capitalize flex-shrink-0">
                          {activeTab.replace("_", " ")} Content
                        </h3>
                        <div className={`flex-1 min-h-0 ${
                          fileContents[activeTab]!.file_type === "json" && activeTab === "main_json"
                            ? "overflow-hidden" 
                            : "overflow-hidden"
                        }`}>
                          {fileContents[activeTab]!.file_type === "json" 
                            ? renderJsonContent(fileContents[activeTab]!.content, activeTab)
                            : renderTextEditor(fileContents[activeTab]!.content, activeTab)
                          }
                        </div>
                      </>
                    ) : (
                      <div className="flex items-center justify-center h-full">
                        <div className="text-center">
                          <svg className="w-12 h-12 text-gray-400 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                          </svg>
                          <p className="text-gray-500 dark:text-gray-400">
                            No content available for {activeTab.replace("_", " ")}
                          </p>
                        </div>
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
          ) : 
          /* COMMENTED OUT - AGENTIC FUNCTIONALITY
          activeMainTab === "result" ? (
            <div className="flex flex-col h-full p-6">
              {/* Result Content - Only Analysis Table *\/}
              <h3 className="text-xl font-semibold text-gray-900 dark:text-white mb-4 flex-shrink-0">
                Analysis Results
              </h3>
              <div className="flex-1 min-h-0 overflow-auto">
                {renderAnalysisTable()}
              </div>
            </div>
          ) : 
          END COMMENTED OUT */ (
            // Manual Checker Content
            <div className="flex flex-col h-full">
              {/* Test Lists Section (Top) */}
              <div className="h-1/2 border-b border-gray-200 dark:border-gray-700">
                <div className="h-full flex">
                  {/* Fail to Pass Tests */}
                  <div className="w-1/2 border-r border-gray-200 dark:border-gray-700 flex flex-col">
                    <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
                      <div className="flex items-center justify-between gap-3">
                        <h4 className="font-medium text-gray-900 dark:text-white text-sm flex-shrink-0">
                          Fail to Pass Tests ({filteredFailToPassTests.length}/{failToPassTests.length})
                        </h4>
                        <input
                          type="text"
                          placeholder="Filter tests..."
                          value={failToPassFilter}
                          onChange={(e) => setFailToPassFilter(e.target.value)}
                          className="flex-1 min-w-0 px-2 py-1 text-xs border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:focus:ring-blue-400"
                        />
                      </div>
                    </div>
                    <div className="flex-1 overflow-auto">
                      {filteredFailToPassTests.length === 0 ? (
                        <div className="px-4 py-8 text-center text-gray-500 dark:text-gray-400 text-sm">
                          {failToPassFilter ? "No tests match the filter" : "No tests available"}
                        </div>
                      ) : (
                        filteredFailToPassTests.map((test, index) => {
                          const testStatus = getTestStatus(test, "f2p");
                          const hasError = hasAnyViolations(test, "f2p");
                          
                          return (
                            <div
                              key={index}
                              id={`fail_to_pass-item-${index}`}
                              className={`px-4 py-1 text-sm border-b border-gray-100 dark:border-gray-600 cursor-pointer flex items-center ${
                                currentSelection === "fail_to_pass" && selectedFailToPassIndex === index
                                  ? "bg-blue-100 dark:bg-blue-900/50 text-blue-900 dark:text-blue-100"
                                  : hasError
                                    ? "bg-red-50 dark:bg-red-900/20 text-gray-700 dark:text-gray-300 hover:bg-red-100 dark:hover:bg-red-900/30"
                                    : "text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
                              }`}
                              onClick={() => {
                                setCurrentSelection("fail_to_pass");
                                setSelectedFailToPassIndex(index);
                                searchForTest(test);
                              }}
                            >
                              <span className="w-8 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0 font-mono text-xs">
                                {index + 1}
                              </span>
                              <span className="flex-1 truncate">{test}</span>
                              <div className="flex items-center gap-1 ml-2 flex-shrink-0">
                                {testStatus && (
                                  <>
                                    {renderStatusIcon(testStatus.base)}
                                    {renderStatusIcon(testStatus.before)}
                                    {renderStatusIcon(testStatus.after)}
                                    {renderStatusIcon(testStatus.agent)}
                                  </>
                                )}
                                {hasError && (
                                  <div className="w-4 h-4 flex items-center justify-center bg-red-100 dark:bg-red-900/50 rounded-full ml-1">
                                    <svg className="w-2.5 h-2.5 text-red-600 dark:text-red-400" fill="currentColor" viewBox="0 0 20 20">
                                      <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                                    </svg>
                                  </div>
                                )}
                              </div>
                            </div>
                          );
                        })
                      )}
                    </div>
                  </div>
                  
                  {/* Pass to Pass Tests */}
                  <div className="w-1/2 flex flex-col">
                    <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
                      <div className="flex items-center justify-between gap-3">
                        <h4 className="font-medium text-gray-900 dark:text-white text-sm flex-shrink-0">
                          Pass to Pass Tests ({filteredPassToPassTests.length}/{passToPassTests.length})
                        </h4>
                        <input
                          type="text"
                          placeholder="Filter tests..."
                          value={passToPassFilter}
                          onChange={(e) => setPassToPassFilter(e.target.value)}
                          className="flex-1 min-w-0 px-2 py-1 text-xs border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:focus:ring-blue-400"
                        />
                      </div>
                    </div>
                    <div className="flex-1 overflow-auto">
                      {filteredPassToPassTests.length === 0 ? (
                        <div className="px-4 py-8 text-center text-gray-500 dark:text-gray-400 text-sm">
                          {passToPassFilter ? "No tests match the filter" : "No tests available"}
                        </div>
                      ) : (
                        filteredPassToPassTests.map((test, index) => {
                          const testStatus = getTestStatus(test, "p2p");
                          const hasError = hasAnyViolations(test, "p2p");
                          
                          return (
                            <div
                              key={index}
                              id={`pass_to_pass-item-${index}`}
                              className={`px-4 py-1 text-sm border-b border-gray-100 dark:border-gray-600 cursor-pointer flex items-center ${
                                currentSelection === "pass_to_pass" && selectedPassToPassIndex === index
                                  ? "bg-green-100 dark:bg-green-900/50 text-green-900 dark:text-green-100"
                                  : hasError
                                    ? "bg-red-50 dark:bg-red-900/20 text-gray-700 dark:text-gray-300 hover:bg-red-100 dark:hover:bg-red-900/30"
                                    : "text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700"
                              }`}
                              onClick={() => {
                                setCurrentSelection("pass_to_pass");
                                setSelectedPassToPassIndex(index);
                                searchForTest(test);
                              }}
                            >
                              <span className="w-8 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0 font-mono text-xs">
                                {index + 1}
                              </span>
                              <span className="flex-1 truncate">{test}</span>
                              <div className="flex items-center gap-1 ml-2 flex-shrink-0">
                                {testStatus && (
                                  <>
                                    {renderStatusIcon(testStatus.base)}
                                    {renderStatusIcon(testStatus.before)}
                                    {renderStatusIcon(testStatus.after)}
                                    {renderStatusIcon(testStatus.agent)}
                                  </>
                                )}
                                {hasError && (
                                  <div className="w-4 h-4 flex items-center justify-center bg-red-100 dark:bg-red-900/50 rounded-full ml-1">
                                    <svg className="w-2.5 h-2.5 text-red-600 dark:text-red-400" fill="currentColor" viewBox="0 0 20 20">
                                      <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                                    </svg>
                                  </div>
                                )}
                              </div>
                            </div>
                          );
                        })
                      )}
                    </div>
                  </div>
                </div>
              </div>
              
              {/* Log Search Results Section (Bottom) */}
              <div className="h-1/2 flex">
                {/* Base Log Results */}
                <div className="w-1/4 border-r border-gray-200 dark:border-gray-700 flex flex-col">
                  <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600 flex items-center justify-between">
                    <h4 className="font-medium text-gray-900 dark:text-white text-sm">
                      Base Log ({searchResults.base.length} results)
                    </h4>
                    {searchResults.base.length > 1 && (
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => handleSearchResultNavigation("base", "prev")}
                          className="px-1 py-0 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          ←
                        </button>
                        <span className="text-xs text-gray-500">
                          {searchResultIndices.base + 1}/{searchResults.base.length}
                        </span>
                        <button
                          onClick={() => handleSearchResultNavigation("base", "next")}
                          className="px-1 py-0 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          →
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="flex-1 overflow-auto p-4">
                    {searchResults.base.length > 0 ? (
                      <div className="font-mono text-xs">
                        {(() => {
                          const result = searchResults.base[searchResultIndices.base];
                          const startLineNumber = result.line_number - result.context_before.length;
                          let currentLineNumber = startLineNumber;
                          
                          return (
                            <>
                              {/* Context before */}
                              {result.context_before.map((line, i) => (
                                <div key={`before-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                              {/* Highlighted match */}
                              <div className="flex bg-yellow-200 dark:bg-yellow-800 text-gray-900 dark:text-gray-100 font-bold">
                                <span className="w-12 text-right pr-2 text-gray-700 dark:text-gray-300 flex-shrink-0">
                                  {currentLineNumber++}
                                </span>
                                <span className="flex-1">{result.line_content}</span>
                              </div>
                              {/* Context after */}
                              {result.context_after.map((line, i) => (
                                <div key={`after-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                            </>
                          );
                        })()}
                      </div>
                    ) : (
                      <div className="text-gray-500 dark:text-gray-400 text-sm">No matches found</div>
                    )}
                  </div>
                </div>
                
                {/* Before Log Results */}
                <div className="w-1/4 border-r border-gray-200 dark:border-gray-700 flex flex-col">
                  <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600 flex items-center justify-between">
                    <h4 className="font-medium text-gray-900 dark:text-white text-sm">
                      Before Log ({searchResults.before.length} results)
                    </h4>
                    {searchResults.before.length > 1 && (
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => handleSearchResultNavigation("before", "prev")}
                          className="px-1 py-0 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          ←
                        </button>
                        <span className="text-xs text-gray-500">
                          {searchResultIndices.before + 1}/{searchResults.before.length}
                        </span>
                        <button
                          onClick={() => handleSearchResultNavigation("before", "next")}
                          className="px-1 py-0 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          →
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="flex-1 overflow-auto p-4">
                    {searchResults.before.length > 0 ? (
                      <div className="font-mono text-xs">
                        {(() => {
                          const result = searchResults.before[searchResultIndices.before];
                          const startLineNumber = result.line_number - result.context_before.length;
                          let currentLineNumber = startLineNumber;
                          
                          return (
                            <>
                              {/* Context before */}
                              {result.context_before.map((line, i) => (
                                <div key={`before-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                              {/* Highlighted match */}
                              <div className="flex bg-yellow-200 dark:bg-yellow-800 text-gray-900 dark:text-gray-100 font-bold">
                                <span className="w-12 text-right pr-2 text-gray-700 dark:text-gray-300 flex-shrink-0">
                                  {currentLineNumber++}
                                </span>
                                <span className="flex-1">{result.line_content}</span>
                              </div>
                              {/* Context after */}
                              {result.context_after.map((line, i) => (
                                <div key={`after-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                            </>
                          );
                        })()}
                      </div>
                    ) : (
                      <div className="text-gray-500 dark:text-gray-400 text-sm">No matches found</div>
                    )}
                  </div>
                </div>
                
                {/* After Log Results */}
                <div className="w-1/4 border-r border-gray-200 dark:border-gray-700 flex flex-col">
                  <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600 flex items-center justify-between">
                    <h4 className="font-medium text-gray-900 dark:text-white text-sm">
                      After Log ({searchResults.after.length} results)
                    </h4>
                    {searchResults.after.length > 1 && (
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => handleSearchResultNavigation("after", "prev")}
                          className="px-1 py-0 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          ←
                        </button>
                        <span className="text-xs text-gray-500">
                          {searchResultIndices.after + 1}/{searchResults.after.length}
                        </span>
                        <button
                          onClick={() => handleSearchResultNavigation("after", "next")}
                          className="px-1 py-0 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          →
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="flex-1 overflow-auto p-4">
                    {searchResults.after.length > 0 ? (
                      <div className="font-mono text-xs">
                        {(() => {
                          const result = searchResults.after[searchResultIndices.after];
                          const startLineNumber = result.line_number - result.context_before.length;
                          let currentLineNumber = startLineNumber;
                          
                          return (
                            <>
                              {/* Context before */}
                              {result.context_before.map((line, i) => (
                                <div key={`before-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                              {/* Highlighted match */}
                              <div className="flex bg-yellow-200 dark:bg-yellow-800 text-gray-900 dark:text-gray-100 font-bold">
                                <span className="w-12 text-right pr-2 text-gray-700 dark:text-gray-300 flex-shrink-0">
                                  {currentLineNumber++}
                                </span>
                                <span className="flex-1">{result.line_content}</span>
                              </div>
                              {/* Context after */}
                              {result.context_after.map((line, i) => (
                                <div key={`after-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                            </>
                          );
                        })()}
                      </div>
                    ) : (
                      <div className="text-gray-500 dark:text-gray-400 text-sm">No matches found</div>
                    )}
                  </div>
                </div>
                
                {/* Agent Log Results */}
                <div className="w-1/4 flex flex-col">
                  <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600 flex items-center justify-between">
                    <h4 className="font-medium text-gray-900 dark:text-white text-sm">
                      Agent Log ({searchResults.agent.length} results)
                    </h4>
                    {searchResults.agent.length > 1 && (
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => handleSearchResultNavigation("agent", "prev")}
                          className="px-1 py-0 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          ←
                        </button>
                        <span className="text-xs text-gray-500">
                          {searchResultIndices.agent + 1}/{searchResults.agent.length}
                        </span>
                        <button
                          onClick={() => handleSearchResultNavigation("agent", "next")}
                          className="px-1 py-0 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                        >
                          →
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="flex-1 overflow-auto p-4">
                    {searchResults.agent.length > 0 ? (
                      <div className="font-mono text-xs">
                        {(() => {
                          const result = searchResults.agent[searchResultIndices.agent];
                          const startLineNumber = result.line_number - result.context_before.length;
                          let currentLineNumber = startLineNumber;
                          
                          return (
                            <>
                              {/* Context before */}
                              {result.context_before.map((line, i) => (
                                <div key={`before-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                              {/* Highlighted match */}
                              <div className="flex bg-yellow-200 dark:bg-yellow-800 text-gray-900 dark:text-gray-100 font-bold">
                                <span className="w-12 text-right pr-2 text-gray-700 dark:text-gray-300 flex-shrink-0">
                                  {currentLineNumber++}
                                </span>
                                <span className="flex-1">{result.line_content}</span>
                              </div>
                              {/* Context after */}
                              {result.context_after.map((line, i) => (
                                <div key={`after-${i}`} className="flex text-gray-500 dark:text-gray-400">
                                  <span className="w-12 text-right pr-2 text-gray-400 dark:text-gray-500 flex-shrink-0">
                                    {currentLineNumber++}
                                  </span>
                                  <span className="flex-1">{line}</span>
                                </div>
                              ))}
                            </>
                          );
                        })()}
                      </div>
                    ) : (
                      <div className="text-gray-500 dark:text-gray-400 text-sm">No matches found</div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full items-center justify-center">
      <div className="w-full max-w-2xl">
        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-8 shadow-sm">
          
          {/* Input Section - Always Visible */}
          <div className="text-center">
            <h2 className="text-3xl font-bold text-gray-900 dark:text-white mb-8">
              Deliverable Checker
            </h2>
            
            <div className="mb-8">
              <input
                type="text"
                value={deliverableLink}
                onChange={(e) => setDeliverableLink(e.target.value)}
                placeholder="Deliverable Link"
                className="w-full px-6 py-4 text-lg border-2 border-gray-300 dark:border-gray-600 rounded-full bg-white dark:bg-gray-700 text-gray-900 dark:text-white placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:border-blue-500 dark:focus:border-blue-400 transition-colors"
                disabled={isProcessing}
              />
            </div>

            <button
              onClick={handleSubmit}
              disabled={isProcessing || !deliverableLink.trim()}
              className="px-8 py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white rounded-full text-lg font-semibold shadow-lg transition-colors disabled:cursor-not-allowed"
            >
              Submit
            </button>

            {error && (
              <div className="mt-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                <p className="text-red-600 dark:text-red-400">{error}</p>
              </div>
            )}
          </div>

          {/* Processing Section - Show Below Input */}
          {isProcessing && (
            <div className="text-center mt-12 pt-8 border-t border-gray-200 dark:border-gray-700">
              <h3 className="text-xl font-semibold text-gray-900 dark:text-white mb-6">
                Processing Deliverable
              </h3>

              <div className="space-y-6">
                {/* Validating */}
                <div className="flex items-center justify-center gap-4">
                  {renderIcon("validating")}
                  <span className={`text-lg font-medium ${getStageTextClass("validating")}`}>
                    Validating
                  </span>
                </div>

                {/* Downloading */}
                <div className="flex items-center justify-center gap-4">
                  {renderIcon("downloading")}
                  <span className={`text-lg font-medium ${getStageTextClass("downloading")}`}>
                    Downloading
                  </span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
